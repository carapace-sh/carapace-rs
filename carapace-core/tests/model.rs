use carapace_core::{ArgumentStyle, Command, Flag, Nargs, Positional};

fn flag(id: &str, name: &str) -> Flag {
    Flag {
        id: id.to_owned(),
        name: name.to_owned(),
        ..Default::default()
    }
}

#[test]
fn lookup_long_and_short() {
    let cmd = Command::new("app")
        .flag(Flag {
            shorthand: Some('v'),
            ..flag("verbose", "verbose")
        })
        .flag(flag("output", "output").clone());
    assert!(cmd.lookup_long("verbose").is_some());
    assert!(cmd.lookup_short('v').is_some());
    assert!(cmd.lookup_short('o').is_none());
    assert!(cmd.lookup_id("output").is_some());
}

#[test]
fn lookup_subcommand_by_alias() {
    let cmd = Command::new("app").subcommand(Command::new("build").alias("b"));
    assert!(cmd.lookup_subcommand("build").is_some());
    assert!(cmd.lookup_subcommand("b").is_some());
    assert!(cmd.lookup_subcommand("x").is_none());
}

#[test]
fn positional_at_resolves_repeatable() {
    let cmd = Command::new("app")
        .positional(Positional {
            name: "first".into(),
            ..Default::default()
        })
        .positional(Positional {
            name: "rest".into(),
            repeatable: true,
            ..Default::default()
        });
    assert_eq!(cmd.positional_at(0).unwrap().name, "first");
    assert_eq!(cmd.positional_at(1).unwrap().name, "rest");
    assert_eq!(cmd.positional_at(7).unwrap().name, "rest");
    assert_eq!(cmd.positional_at(1).unwrap().repeatable, true);
}

#[test]
fn flag_consumes_exact_nargs() {
    let f = Flag {
        takes_value: true,
        nargs: Nargs::Exact(2),
        ..Default::default()
    };
    assert!(f.consumes(0, "one"));
    assert!(f.consumes(1, "two"));
    assert!(!f.consumes(2, "three"));
}

#[test]
fn flag_consumes_optarg_never_takes_positional() {
    let f = Flag {
        takes_value: true,
        optional: true,
        ..Default::default()
    };
    assert!(!f.consumes(0, "anything"));
}

#[test]
fn flag_consumes_variadic_stops_at_flag() {
    let f = Flag {
        takes_value: true,
        nargs: Nargs::Any,
        ..Default::default()
    };
    assert!(f.consumes(0, "value"));
    assert!(f.consumes(1, "another"));
    assert!(!f.consumes(2, "--next-flag"));
}

#[test]
fn bool_flag_never_consumes() {
    let f = Flag {
        takes_value: false,
        ..Default::default()
    };
    assert!(!f.consumes(0, "value"));
}

#[test]
fn delimiter_enabled() {
    let f = Flag {
        optarg_delimiter: '=',
        ..Default::default()
    };
    assert!(f.delimiter_enabled());
    let disabled = Flag {
        optarg_delimiter: '\u{1}',
        ..Default::default()
    };
    assert!(!disabled.delimiter_enabled());
}

#[test]
fn default_argument_style_accepts_all() {
    assert_eq!(Flag::default().argument_style, ArgumentStyle::ALL);
}
