use carapace_rs::{Action, Context, config};

fn value_of(output: &str) -> String {
    output.trim_end_matches('\n').to_owned()
}

#[test]
fn styles_load_from_json() {
    let dir = std::env::temp_dir().join(format!("carapace-config-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("carapace")).unwrap();
    std::fs::write(
        dir.join("carapace/styles.json"),
        r#"{"carapace.Error": "magenta", "keyword": "blue bold"}"#,
    )
    .unwrap();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
    config::reset_styles();

    let styles = config::styles();
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    config::reset_styles(); // other tests start from defaults
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(styles.carapace.error, "magenta");
    assert_eq!(
        styles.custom.get("keyword").map(String::as_str),
        Some("blue bold")
    );
}

#[test]
fn color_priority() {
    let env = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<std::collections::HashMap<String, String>>()
    };
    assert!(
        config::Flags::from_env_map(&env(&[("CARAPACE_COLOR", "0"), ("NO_COLOR", "1")]))
            .color_disabled
    );
    assert!(
        !config::Flags::from_env_map(&env(&[("CARAPACE_COLOR", "1"), ("NO_COLOR", "1")]))
            .color_disabled,
        "explicit CARAPACE_COLOR wins"
    );
    assert!(
        !config::Flags::from_env_map(&env(&[("NO_COLOR", "0")])).color_disabled,
        "NO_COLOR=0 is falsy"
    );
    assert!(
        config::Flags::from_env_map(&env(&[("CLICOLOR", "0")])).color_disabled,
        "CLICOLOR=0 disables color"
    );
}

#[test]
fn hidden_modes() {
    let env = |value: &str| {
        [("CARAPACE_HIDDEN".to_owned(), value.to_owned())]
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>()
    };
    assert_eq!(
        config::Flags::from_env_map(&std::collections::HashMap::new()).hidden,
        config::Hidden::None
    );
    assert_eq!(
        config::Flags::from_env_map(&env("1")).hidden,
        config::Hidden::ExcludeCarapace
    );
    assert_eq!(
        config::Flags::from_env_map(&env("2")).hidden,
        config::Hidden::IncludeCarapace
    );
}

#[test]
fn exec_command_completes_from_stdout() {
    let action = Action::exec_command("echo", &["hello"], |stdout| {
        let word = String::from_utf8_lossy(stdout).trim().to_owned();
        Action::values([word])
    });
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "hello");
}

#[test]
fn exec_command_error_becomes_message() {
    let action = Action::exec_command("false", &[], |_| Action::default());
    let invoked = action.invoke(&Context::default());
    assert!(!invoked.meta.messages.is_empty());
}

#[test]
fn exec_command_e_exposes_error() {
    let action = Action::exec_command_e("false", &[], |result| match result {
        Ok(_) => Action::values(["ok"]),
        Err(_) => Action::values(["failed"]),
    });
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "failed");
}

#[test]
fn exec_respects_context_dir_and_env() {
    let action = Action::exec_command("pwd", &[], |stdout| {
        Action::values([value_of(&String::from_utf8_lossy(stdout))])
    });
    let invoked = action.invoke(&Context::default().dir("/tmp"));
    assert_eq!(invoked.values[0].value, "/tmp");
}

#[test]
fn style_config_lists_entries() {
    let invoked = Action::style_config().invoke(&Context::default());
    let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
    assert!(values.iter().any(|v| v.starts_with("carapace.Error=")));
    assert!(values.iter().any(|v| v.starts_with("carapace.Usage=")));
}

#[test]
fn styles_action_lists_known_names() {
    let invoked = Action::styles(Vec::<String>::new()).invoke(&Context::default());
    assert!(invoked.values.iter().any(|v| v.value == "blue"));
    assert!(invoked.values.iter().any(|v| v.value == "underlined"));
    assert!(invoked.values.iter().any(|v| v.value == "bg-magenta"));
    // the 256-color ranges stay collapsed until the word starts with them
    assert!(invoked.values.iter().any(|v| v.value == "color"));
    assert!(!invoked.values.iter().any(|v| v.value == "color12"));
}

#[test]
fn styles_action_expands_256_colors_contextually() {
    let invoked = Action::styles(Vec::<String>::new()).invoke(&Context::default().value("color1"));
    let values: Vec<String> = invoked.values.into_iter().map(|v| v.value).collect();
    assert!(values.contains(&"color12".to_owned()));
    assert!(!values.contains(&"color".to_owned()));
    // the background half is unaffected by the foreground word
    assert!(values.iter().any(|v| v == "bg-color"));
}

#[test]
fn styles_action_combines_existing_styles() {
    let invoked = Action::styles(["bold"]).invoke(&Context::default());
    let blue = invoked
        .values
        .iter()
        .find(|v| v.value == "blue")
        .expect("blue candidate");
    assert_eq!(blue.style, "bold blue");
}

#[test]
fn styles_action_suppresses_taken_halves() {
    let invoked = Action::styles(["blue"]).invoke(&Context::default());
    let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
    assert!(!values.contains(&"red"), "foreground taken");
    assert!(values.contains(&"bg-red"), "background still offered");
}

#[test]
fn case_insensitive_matching() {
    let values = vec![carapace_rs::RawValue::new("CamelCase")];
    unsafe { std::env::remove_var("CARAPACE_MATCH") };
    // case-sensitive by default
    assert!(
        !carapace_rs::shell::Shell::Fish
            .value(
                "camel",
                &carapace_rs::Meta::default(),
                &values,
                &carapace_rs::shell::bash::Context::default(),
            )
            .contains("CamelCase")
    );
    unsafe { std::env::set_var("CARAPACE_MATCH", "CASE_INSENSITIVE") };
    let matched = carapace_rs::shell::Shell::Fish
        .value(
            "camel",
            &carapace_rs::Meta::default(),
            &values,
            &carapace_rs::shell::bash::Context::default(),
        )
        .contains("CamelCase");
    unsafe { std::env::remove_var("CARAPACE_MATCH") };
    assert!(matched);
}

#[test]
fn messages_dedup_on_merge() {
    let mut meta = carapace_rs::Meta::default().message("same");
    meta.merge(&carapace_rs::Meta::default().message("same"));
    assert_eq!(meta.messages, vec!["same"]);
}

#[test]
fn exec_passes_context_env_not_process_env() {
    // Go TestActionExecCommandEnv: context-set variables reach the child
    let action = Action::exec_command("sh", &["-c", "echo $carapace_exec_env_test"], |stdout| {
        Action::values([value_of(&String::from_utf8_lossy(stdout))])
    });
    let invoked = action.invoke(&Context::default().env("carapace_exec_env_test", "test"));
    assert_eq!(invoked.values[0].value, "test");
}

#[test]
fn keyword_styles_follow_go_table() {
    use carapace_rs::config::keyword_style;
    assert_eq!(keyword_style("true"), "green");
    assert_eq!(keyword_style("FALSE"), "red");
    assert_eq!(keyword_style("auto"), "yellow");
    assert_eq!(keyword_style("unknown"), "dim white");
    // dashes and underscores are ignored, matching is case-insensitive
    assert_eq!(keyword_style("In-Progress"), "yellow");
    assert_eq!(keyword_style("NOT_EXISTS"), "red");
    assert_eq!(keyword_style("whatever"), "");
}

#[test]
fn style_config_lists_carapace_entries() {
    let action = Action::style_config();
    let values: Vec<String> = action
        .invoke(&Context::default())
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert!(values.iter().any(|v| v == "carapace.FlagArg=blue"));
    assert!(values.iter().any(|v| v == "carapace.FlagOptArg=yellow"));
    assert!(values.iter().any(|v| v == "carapace.FlagMultiArg=magenta"));
    assert!(
        values
            .iter()
            .any(|v| v.starts_with("carapace.KeywordPositive="))
    );
}

#[test]
fn highlight_and_loglevel_styles() {
    use carapace_rs::config::{for_log_level, styles};
    let carapace = styles().carapace;
    assert_eq!(carapace.highlight(0), "blue");
    assert_eq!(carapace.highlight(11), "dim bold");
    assert_eq!(carapace.highlight(12), "", "out of range is default");

    assert_eq!(for_log_level("INFO"), "green");
    assert_eq!(for_log_level("vdebug"), "dim white");
    assert_eq!(for_log_level("panic"), "cyan");
    assert_eq!(for_log_level("nosuch"), "");
}

#[test]
fn style_config_lists_highlight_and_loglevel() {
    carapace_rs::config::reset_styles();
    let action = Action::style_config();
    let values: Vec<String> = action
        .invoke(&Context::default())
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert!(values.iter().any(|v| v == "carapace.Highlight1=blue"));
    assert!(values.iter().any(|v| v == "carapace.Highlight12=dim bold"));
    assert!(
        values
            .iter()
            .any(|v| v == "carapace.LogLevelWarning=yellow")
    );
}
