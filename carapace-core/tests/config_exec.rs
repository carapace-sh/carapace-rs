use carapace_core::{Action, Context, config};

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
    unsafe { std::env::set_var("CARAPACE_COLOR", "0") };
    unsafe { std::env::set_var("NO_COLOR", "1") };
    assert!(config::color_disabled());

    unsafe { std::env::set_var("CARAPACE_COLOR", "1") };
    unsafe { std::env::set_var("NO_COLOR", "1") };
    assert!(!config::color_disabled(), "explicit CARAPACE_COLOR wins");

    unsafe { std::env::remove_var("CARAPACE_COLOR") };
    unsafe { std::env::set_var("NO_COLOR", "0") };
    assert!(!config::color_disabled(), "NO_COLOR=0 is falsy");

    unsafe { std::env::remove_var("NO_COLOR") };
    unsafe { std::env::set_var("CLICOLOR", "0") };
    assert!(config::color_disabled());

    unsafe { std::env::remove_var("CLICOLOR") };
}

#[test]
fn hidden_modes() {
    unsafe { std::env::remove_var("CARAPACE_HIDDEN") };
    assert_eq!(config::hidden(), config::Hidden::None);
    unsafe { std::env::set_var("CARAPACE_HIDDEN", "2") };
    assert_eq!(config::hidden(), config::Hidden::IncludeCarapace);
    unsafe { std::env::remove_var("CARAPACE_HIDDEN") };
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
    let invoked = Action::styles().invoke(&Context::default());
    assert!(invoked.values.iter().any(|v| v.value == "blue"));
    assert!(invoked.values.iter().any(|v| v.value == "underlined"));
}

#[test]
fn case_insensitive_matching() {
    let values = vec![carapace_core::RawValue::new("CamelCase")];
    unsafe { std::env::remove_var("CARAPACE_MATCH") };
    // case-sensitive by default
    assert!(
        !carapace_core::shell::Shell::Fish
            .value("camel", &carapace_core::Meta::default(), &values)
            .contains("CamelCase")
    );
    unsafe { std::env::set_var("CARAPACE_MATCH", "CASE_INSENSITIVE") };
    let matched = carapace_core::shell::Shell::Fish
        .value("camel", &carapace_core::Meta::default(), &values)
        .contains("CamelCase");
    unsafe { std::env::remove_var("CARAPACE_MATCH") };
    assert!(matched);
}

#[test]
fn messages_dedup_on_merge() {
    let mut meta = carapace_core::Meta::default().message("same");
    meta.merge(&carapace_core::Meta::default().message("same"));
    assert_eq!(meta.messages, vec!["same"]);
}
