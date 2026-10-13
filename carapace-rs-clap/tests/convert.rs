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

    let values: Vec<String> =
        carapace_rs::traverse::action_commands(&model, std::slice::from_ref(&model.name))
            .invoke(&Context::default().env("CARAPACE_HIDDEN", "2"))
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
    assert!(values.contains(&"secret".to_string()));
}

#[test]
fn hidden_flags_respect_hidden_env() {
    let mut cmd = Command::new("app").arg(
        Arg::new("secret")
            .long("secret")
            .hide(true)
            .action(ArgAction::SetTrue),
    );
    let model = to_model(&mut cmd);

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

    let values: Vec<String> =
        carapace_rs::traverse::action_flags(&model, std::slice::from_ref(&model.name))
            .invoke(&Context::default().value("--").env("CARAPACE_HIDDEN", "2"))
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
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

#[test]
fn converts_visible_aliases() {
    // clap's `get_aliases()` returns hidden aliases only; visible aliases
    // are user-facing flag spellings and must be known to the model (Go
    // resolves alias spellings for lookup and changed-tracking, while
    // flag-name completion only offers the longhand and shorthand)
    let cmd = Command::new("app").arg(
        Arg::new("verbose")
            .long("verbose")
            .visible_alias("loud")
            .alias("quiet")
            .action(ArgAction::SetTrue),
    );

    let model = to_model(&mut cmd.clone());
    let verbose = model.lookup_long("verbose").unwrap();
    assert!(verbose.aliases.contains(&"loud".to_owned()));
    assert!(verbose.aliases.contains(&"quiet".to_owned()));

    // alias spellings resolve to the flag and count as given
    assert!(model.lookup_long("loud").is_some());
    let completion = carapace_rs::traverse(
        &model,
        vec!["--loud".to_owned(), "--verbose".to_owned(), "".to_owned()],
    );
    assert_eq!(
        completion
            .context
            .parsed_flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["verbose"]
    );
}

#[test]
fn global_args_are_inherited_by_subcommands() {
    // clap propagates global args at parse time, not in the built tree;
    // the model must copy them down like cobra persistent flags
    let cmd = Command::new("app")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .subcommand(Command::new("sub"));

    let model = to_model(&mut cmd.clone());
    let sub = model.lookup_subcommand("sub").unwrap();
    assert!(sub.lookup_long("verbose").is_some());
}

#[test]
fn variadic_flag_consumes_values() {
    // `num_args(0..)` is variadic, not an optarg: it consumes values until
    // the next flag-lookalike
    let cmd = Command::new("app").arg(
        Arg::new("set")
            .long("set")
            .action(ArgAction::Set)
            .num_args(0..),
    );

    let model = to_model(&mut cmd.clone());
    let flag = model.lookup_long("set").unwrap();
    assert!(flag.takes_value);
    assert!(!flag.optional);
    assert_eq!(flag.nargs, Nargs::Any);
}

#[test]
fn pre_invoke_hooks_nest_ancestors_outermost() {
    // Go `storage.preinvoke`: the parent's hook wraps the child's, so
    // ancestors run first and the command's own hook sees their result
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let order = Arc::new(AtomicUsize::new(0));
    let parent_seen = order.clone();
    let own_seen = order.clone();

    let mut cmd = Command::new("app").subcommand(
        Command::new("sub")
            .arg(Arg::new("name").long("name").action(ArgAction::Set))
            .pre_invoke(Arc::new(move |_: &carapace_rs::Context, action| {
                let seen = own_seen.clone();
                Action::callback(move |c| {
                    let invoked = action.clone().invoke(c);
                    assert_eq!(seen.fetch_add(1, Ordering::SeqCst), 1);
                    invoked.into()
                })
            })),
    );
    cmd.clone()
        .pre_invoke(Arc::new(move |_: &carapace_rs::Context, action| {
            let seen = parent_seen.clone();
            Action::callback(move |c| {
                let invoked = action.clone().invoke(c);
                assert_eq!(seen.fetch_add(1, Ordering::SeqCst), 0);
                invoked.into()
            })
        }));

    let model = to_model(&mut cmd);
    let completion = carapace_rs::traverse(
        &model,
        vec!["sub".to_owned(), "--name".to_owned(), "".to_owned()],
    );
    let _ = completion.action.invoke(&Context::default());
}

#[test]
fn value_delimiter_completes_last_segment() {
    let cmd = Command::new("app").arg(
        Arg::new("list")
            .long("list")
            .action(ArgAction::Set)
            .value_delimiter(',')
            .add(carapace_rs_clap::ArgCompleter::new(Action::values([
                "one", "two", "three",
            ]))),
    );
    let model = to_model(&mut cmd.clone());

    // attached: `--list=one,` completes after the last delimiter
    let completion = carapace_rs::traverse(&model, vec!["--list=one,".to_owned()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(values.contains(&"--list=one,two".to_owned()), "{values:?}");

    // next-arg: `--list one,` completes the trailing segment
    let completion = carapace_rs::traverse(&model, vec!["--list".to_owned(), "one,".to_owned()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(values.contains(&"one,two".to_owned()), "{values:?}");
}

#[test]
fn subcommand_short_flag_selects_subcommand() {
    let cmd = Command::new("app")
        .subcommand(Command::new("list").short_flag('l'))
        .subcommand(Command::new("other"));
    let model = to_model(&mut cmd.clone());

    // the shorthand token dispatches into the subcommand
    let completion = carapace_rs::traverse(&model, vec!["-l".to_owned(), "".to_owned()]);
    let invoked = completion.action.invoke(&completion.context);
    // completing inside `list` (no positionals defined): no crash is the
    // dispatch proof; the path reflects the subcommand
    assert_eq!(model.lookup_subcommand("-l").unwrap().name, "list");
    assert_eq!(model.lookup_subcommand("list").unwrap().name, "list");
    let _ = invoked;
}

#[test]
fn command_value_hint_completes_executables() {
    let cmd = Command::new("app")
        .arg(Arg::new("cmd").value_hint(ValueHint::CommandString))
        .arg(Arg::new("dir").value_hint(ValueHint::DirPath));
    let model = to_model(&mut cmd.clone());

    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(
        values.iter().all(|v| !v.starts_with('-')),
        "executables expected, got {values:?}"
    );
}

#[test]
fn bounded_nargs_consumes_minimum_and_stops_at_flags() {
    // clap `num_args(2..=4)`: the first two values are consumed
    // unconditionally; beyond the minimum a flag-lookalike stops the
    // consumption (same shape as the engine's variadic handling)
    let cmd = Command::new("app")
        .arg(
            Arg::new("pair")
                .long("pair")
                .action(ArgAction::Append)
                .num_args(2..=4),
        )
        .arg(Arg::new("other").long("other").action(ArgAction::SetTrue));
    let model = to_model(&mut cmd.clone());
    assert!(model.lookup_long("pair").is_some());

    let completion = carapace_rs::traverse(
        &model,
        vec![
            "--pair".to_owned(),
            "a".to_owned(),
            "b".to_owned(),
            "--".to_owned(),
        ],
    );
    assert_eq!(
        completion.context.parsed_flags,
        vec![("pair".to_owned(), vec!["a".to_owned(), "b".to_owned()])]
    );

    // a known flag stops the bounded consumption above the minimum
    let completion = carapace_rs::traverse(
        &model,
        vec![
            "--pair".to_owned(),
            "a".to_owned(),
            "b".to_owned(),
            "--other".to_owned(),
            "".to_owned(),
        ],
    );
    assert_eq!(
        completion.context.parsed_flags,
        vec![
            ("pair".to_owned(), vec!["a".to_owned(), "b".to_owned()]),
            ("other".to_owned(), Vec::<String>::new()),
        ]
    );
}

#[test]
fn positional_completion_pads_virtual_positionals() {
    // more actions than defined positionals: virtual positionals absorb
    // the remaining actions (Go's storage is independent of the arg list)
    let cmd = Command::new("app")
        .arg(Arg::new("first").index(1))
        .positional_completion([
            Action::values(["first"]),
            Action::values(["second"]),
            Action::values(["third"]),
        ]);
    let model = to_model(&mut cmd.clone());

    assert_eq!(model.positionals.len(), 3);
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
    // virtual positionals are optional (completing beyond them stays legal)
    assert!(model.positionals[2].optional);
}

#[test]
fn command_completer_macro_attaches_positional_any() {
    // the macro wraps the action for clap derive's `add = <expr>` escape hatch
    let cmd = Command::new("app").add(carapace_rs_clap::command_completer!(Action::values([
        "via-macro"
    ])));
    let model = to_model(&mut cmd.clone());
    let invoked = model
        .positional_any
        .as_ref()
        .expect("command_completer! must fill positional_any")
        .invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "via-macro");
}
