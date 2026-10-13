use carapace_rs::{Action, Context, config};

fn value_of(output: &str) -> String {
    output.trim_end_matches('\n').to_owned()
}

/// Serializes tests that mutate the process environment.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Saves an environment variable and restores it on drop (panic-safe).
struct EnvGuard(Option<String>, &'static str);

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var(name).ok();
        unsafe { std::env::set_var(name, value) };
        Self(previous, name)
    }

    fn remove(name: &'static str) -> Self {
        let previous = std::env::var(name).ok();
        unsafe { std::env::remove_var(name) };
        Self(previous, name)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(value) => unsafe { std::env::set_var(self.1, value) },
            None => unsafe { std::env::remove_var(self.1) },
        }
    }
}

#[test]
fn styles_load_from_json() {
    let _lock = env_lock();
    let dir = std::env::temp_dir().join(format!("carapace-config-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("carapace")).unwrap();
    std::fs::write(
        dir.join("carapace/styles.json"),
        r#"{"carapace.Error": "magenta", "keyword": "blue bold"}"#,
    )
    .unwrap();
    let _guard = EnvGuard::set("XDG_CONFIG_HOME", dir.to_string_lossy().as_ref());
    config::reset_styles();

    let styles = config::styles();
    drop(_guard);
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
    let action = Action::exec_command("echo", ["hello"], |stdout| {
        let word = String::from_utf8_lossy(stdout).trim().to_owned();
        Action::values([word])
    });
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.to_export().values[0].value, "hello");
}

#[test]
fn exec_command_error_becomes_message() {
    let action = Action::exec_command("false", [""; 0], |_| Action::default());
    let invoked = action.invoke(&Context::default());
    assert!(!invoked.to_export().meta.messages.is_empty());
}

#[test]
fn exec_command_e_exposes_error() {
    let action = Action::exec_command_e("false", [""; 0], |result| match result {
        Ok(_) => Action::values(["ok"]),
        Err(_) => Action::values(["failed"]),
    });
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.to_export().values[0].value, "failed");
}

#[test]
fn exec_respects_context_dir_and_env() {
    let action = Action::exec_command("pwd", [""; 0], |stdout| {
        Action::values([value_of(&String::from_utf8_lossy(stdout))])
    });
    let invoked = action.invoke(&Context::default().dir("/tmp"));
    assert_eq!(invoked.to_export().values[0].value, "/tmp");
}

#[test]
fn style_config_lists_configs() {
    let invoked = Action::style_config().invoke(&Context::default().value(""));
    let export = invoked.to_export();
    let values: Vec<&str> = export.values.iter().map(|v| v.value.as_str()).collect();
    assert_eq!(values, vec!["carapace."]);
}

#[test]
fn styles_action_lists_known_names() {
    let invoked = Action::styles(Vec::<String>::new()).invoke(&Context::default());
    assert!(invoked.to_export().values.iter().any(|v| v.value == "blue"));
    assert!(
        invoked
            .to_export()
            .values
            .iter()
            .any(|v| v.value == "underlined")
    );
    assert!(
        invoked
            .to_export()
            .values
            .iter()
            .any(|v| v.value == "bg-magenta")
    );
    // the 256-color ranges stay collapsed until the word starts with them
    assert!(
        invoked
            .to_export()
            .values
            .iter()
            .any(|v| v.value == "color")
    );
    assert!(
        !invoked
            .to_export()
            .values
            .iter()
            .any(|v| v.value == "color12")
    );
}

#[test]
fn styles_action_expands_256_colors_contextually() {
    let invoked = Action::styles(Vec::<String>::new()).invoke(&Context::default().value("color1"));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .cloned()
        .map(|v| v.value)
        .collect();
    assert!(values.contains(&"color12".to_owned()));
    assert!(!values.contains(&"color".to_owned()));
    // the background half is unaffected by the foreground word
    assert!(values.iter().any(|v| v == "bg-color"));
}

#[test]
fn styles_action_combines_existing_styles() {
    let invoked = Action::styles(["bold"]).invoke(&Context::default());
    let export = invoked.to_export();
    let blue = export
        .values
        .iter()
        .find(|v| v.value == "blue")
        .expect("blue candidate");
    assert_eq!(blue.style, "bold blue");
}

#[test]
fn styles_action_suppresses_taken_halves() {
    let invoked = Action::styles(["blue"]).invoke(&Context::default());
    let export = invoked.to_export();
    let values: Vec<&str> = export.values.iter().map(|v| v.value.as_str()).collect();
    assert!(!values.contains(&"red"), "foreground taken");
    assert!(values.contains(&"bg-red"), "background still offered");
}

#[test]
fn case_insensitive_matching() {
    let _lock = env_lock();
    let values = vec![carapace_rs::RawValue::new("CamelCase")];
    let _guard = EnvGuard::remove("CARAPACE_MATCH");
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
    let _set = EnvGuard::set("CARAPACE_MATCH", "CASE_INSENSITIVE");
    let matched = carapace_rs::shell::Shell::Fish
        .value(
            "camel",
            &carapace_rs::Meta::default(),
            &values,
            &carapace_rs::shell::bash::Context::default(),
        )
        .contains("CamelCase");
    drop(_set);
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
    let action = Action::exec_command("sh", ["-c", "echo $carapace_exec_env_test"], |stdout| {
        Action::values([value_of(&String::from_utf8_lossy(stdout))])
    });
    let invoked = action.invoke(&Context::default().env("carapace_exec_env_test", "test"));
    assert_eq!(invoked.to_export().values[0].value, "test");
}

#[test]
fn keyword_styles_follow_go_table() {
    use carapace_rs::style::keyword_style;
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
    let invoked = Action::style_config().invoke(&Context::default().value("carapace."));
    let export = invoked.to_export();
    let flag_arg = export
        .values
        .iter()
        .find(|v| v.value == "carapace.FlagArg=")
        .expect("flag styles listed");
    assert_eq!(flag_arg.description, "flag with argument");
    assert_eq!(flag_arg.style, "blue");
    assert_eq!(flag_arg.tag, "flag styles");
    assert!(
        export
            .values
            .iter()
            .any(|v| v.value == "carapace.FlagOptArg=")
    );
    assert!(
        export
            .values
            .iter()
            .any(|v| v.value == "carapace.KeywordPositive=")
    );
}

#[test]
fn highlight_and_loglevel_styles() {
    use carapace_rs::config::styles;
    use carapace_rs::style::for_log_level;
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
    let invoked = Action::style_config().invoke(&Context::default().value("carapace."));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .cloned()
        .map(|v| v.value)
        .collect();
    assert!(values.iter().any(|v| v == "carapace.Highlight1="));
    assert!(values.iter().any(|v| v == "carapace.Highlight12="));
    assert!(values.iter().any(|v| v == "carapace.LogLevelWarning="));
}

#[test]
fn style_config_completes_style_values() {
    let invoked = Action::style_config().invoke(&Context::default().value("carapace.Error="));
    let export = invoked.to_export();
    assert!(
        export
            .values
            .iter()
            .any(|v| v.value == "carapace.Error=blue")
    );
    assert!(
        export
            .values
            .iter()
            .any(|v| v.value == "carapace.Error=bold")
    );
    assert!(export.meta.nospace.contains('*'));
}

#[test]
fn style_config_completes_style_combinations() {
    let invoked = Action::style_config().invoke(&Context::default().value("carapace.Error=bold,"));
    let export = invoked.to_export();
    assert!(
        export
            .values
            .iter()
            .any(|v| v.value == "carapace.Error=bold,red")
    );
    assert!(
        !export
            .values
            .iter()
            .any(|v| v.value == "carapace.Error=bold,bold")
    );
}

#[test]
fn style_config_unknown_config_is_a_message() {
    let invoked = Action::style_config().invoke(&Context::default().value("unknown."));
    assert_eq!(
        invoked.to_export().meta.messages,
        vec!["unknown config: 'unknown'".to_owned()]
    );
}
