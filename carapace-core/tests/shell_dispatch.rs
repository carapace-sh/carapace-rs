use carapace_core::shell::{Shell, zsh};
use carapace_core::{Action, Context, Meta, RawValue};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serializes tests that touch CARAPACE_* environment variables.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn clear_compline() {
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
}

fn values() -> carapace_core::RawValues {
    vec![
        RawValue::described("apple", "a fruit"),
        RawValue::described("banana", "a fruit"),
        RawValue {
            description: "no desc".into(),
            ..RawValue::new("cherry")
        },
    ]
}

fn zsh_groups(output: &str) -> Vec<(String, Vec<String>, Vec<String>)> {
    let sections: Vec<&str> = output.split('\u{1}').collect();
    let blocks = sections[3].strip_suffix('\u{2}').unwrap_or(sections[3]);
    blocks
        .split('\u{2}')
        .filter(|block| !block.is_empty())
        .map(|block| {
            let mut parts = block.split('\u{3}');
            let tag = parts.next().unwrap().to_owned();
            let displays: Vec<String> = parts.next().unwrap().lines().map(String::from).collect();
            let values: Vec<String> = parts.next().unwrap().lines().map(String::from).collect();
            (tag, displays, values)
        })
        .collect()
}

#[test]
fn fish_tab_separated() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let output = Shell::Fish.value("", &Meta::default(), &values());
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "apple\ta fruit");
    assert_eq!(lines[2], "cherry\tno desc");
}

#[test]
fn fish_description_length_env() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_DESCRIPTION_LENGTH", "5") };
    let output = Shell::Fish.value("", &Meta::default(), &values());
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    assert_eq!(output.lines().next().unwrap(), "apple\ta ...");
}

#[test]
fn prefix_filtering() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let output = Shell::Fish.value("b", &Meta::default(), &values());
    assert_eq!(output, "banana\ta fruit");
}

#[test]
fn messages_integrated_as_values() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let meta = Meta::default().message("something broke");
    let output = Shell::Fish.value("", &meta, &values());
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.iter().any(|line| line.contains("something broke")));
    assert!(lines.iter().any(|line| line.starts_with("ERR\t")));
}

#[test]
fn message_nospace_propagation() {
    let meta = Meta::default().message("oops");
    let output = Shell::Bash.value("", &meta, &vec![RawValue::new("x")]);
    let (nospace, _) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
}

#[test]
fn zsh_tag_groups_and_quoting() {
    let _guard = env_lock();
    clear_compline();
    let mut tagged = values();
    for value in &mut tagged {
        value.tag = "flags".to_owned();
    }
    let output = Shell::Zsh.value("", &Meta::default(), &tagged);
    let groups = zsh_groups(&output);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "flags");
    assert_eq!(groups[0].2, vec!["apple ", "banana ", "cherry "]);
    assert!(groups[0].1[0].starts_with("apple:a fruit"));
}

#[test]
fn zsh_flag_merge_implicit() {
    let _guard = env_lock();
    clear_compline();
    let mut tagged = values();
    tagged[0].tag = "shorthand flags".to_owned();
    tagged[1].tag = "longhand flags".to_owned();
    let output = Shell::Zsh.value("", &Meta::default(), &tagged);
    let groups = zsh_groups(&output);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "flags");
    assert_eq!(groups[1].0, "default");
}

#[test]
fn zsh_double_quote_state() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "cmd \"ap") };
    let output = Shell::Zsh.value("ap", &Meta::default(), &values());
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
    let groups = zsh_groups(&output);
    assert_eq!(groups[0].2[0], "apple\" ");
}

#[test]
fn zsh_single_quote_state() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "cmd 'ap") };
    let output = Shell::Zsh.value("ap", &Meta::default(), &values());
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
    let groups = zsh_groups(&output);
    assert_eq!(groups[0].2[0], "apple' ");
}

#[test]
fn zsh_escapes_special_chars() {
    let _guard = env_lock();
    clear_compline();
    let output = Shell::Zsh.value(
        "",
        &Meta::default(),
        &vec![RawValue::new("a:b"), RawValue::new("a b")],
    );
    let groups = zsh_groups(&output);
    assert_eq!(groups[0].2[0], "a\\\\ b ");
    assert_eq!(groups[0].2[1], "a\\:b ");
}

#[test]
fn zsh_message_section() {
    let _guard = env_lock();
    clear_compline();
    let meta = Meta::default().message("broken");
    let output = Shell::Zsh.value("", &meta, &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert!(sections[1].contains("broken"));
}

#[test]
fn zsh_zstyle_section() {
    let _guard = env_lock();
    clear_compline();
    let output = Shell::Zsh.value("", &Meta::default(), &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert!(sections[0].contains("=(#b)(apple)"));
}

#[test]
fn sorted_by_display() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let unsorted = vec![RawValue::new("b"), RawValue::new("a")];
    let output = Shell::Fish.value("", &Meta::default(), &unsorted);
    assert_eq!(output, "a\t\nb\t");
}

#[test]
fn export_shell_native_messages() {
    let meta = Meta::default().message("kept in meta");
    let output = Shell::Export.value("", &meta, &values());
    let parsed = carapace_core::export::Export::from_json(&output).unwrap();
    assert_eq!(parsed.meta.messages, vec!["kept in meta"]);
    assert!(parsed.values.iter().all(|v| !v.value.starts_with("ERR")));
}

#[test]
fn nospace_env_propagation() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_NOSPACE", "/") };
    let output = Shell::Bash.value("", &Meta::default(), &vec![RawValue::new("dir/")]);
    unsafe { std::env::remove_var("CARAPACE_NOSPACE") };
    let (nospace, _) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
}

#[test]
fn multiparts_nospace_through_dispatch() {
    let action = Action::multiparts("/", |c| {
        let values: &[&str] = if c.value.is_empty() { &["home", "etc"] } else { &["x"] };
        Action::values(values.iter().copied())
    });
    let invoked = action.invoke(&Context::default().value(""));
    let output = Shell::Bash.value("", &invoked.meta, &invoked.values);
    let (nospace, data) = output.split_once('\u{1}').unwrap();
    // values do not end with the nospace suffix "/" itself
    assert_eq!(nospace, "false");
    assert_eq!(data, "etc\nhome");
}

#[test]
fn zsh_snippet_compiles_structure() {
    let snippet = zsh::snippet("app");
    assert!(snippet.starts_with("#compdef app"));
    assert!(snippet.contains("compdef _app_completion app"));
    assert!(snippet.contains("_describe -t"));
}

#[test]
fn elvish_json_candidates() {
    let _guard = env_lock();
    clear_compline();
    let mut value = RawValue::described("dir", "desc");
    value.tag = "paths".into();
    let nospace_value = RawValue::new("file/").nospace(true);
    let output = Shell::Elvish.value("", &Meta::default(), &vec![value, nospace_value]);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    let candidates = json["Candidates"].as_array().unwrap();
    assert_eq!(candidates[0]["Value"], "dir");
    assert_eq!(candidates[0]["CodeSuffix"], " ");
    assert_eq!(candidates[1]["CodeSuffix"], "");
    assert!(json["Messages"].as_array().unwrap().is_empty());
}

#[test]
fn elvish_native_messages() {
    let meta = Meta::default().message("kept");
    let output = Shell::Elvish.value("", &meta, &values());
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json["Messages"][0], "kept");
}

#[test]
fn nushell_quoting_and_style() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![
        RawValue::new("plain"),
        RawValue::new("with space"),
        RawValue::new("~/tilde path"),
        RawValue::styled("styled", "blue"),
    ];
    let output = Shell::Nushell.value("", &Meta::default(), &values);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["value"], "plain ");
    assert_eq!(json[1]["style"]["fg"], "blue");
    assert_eq!(json[2]["value"], "\"with space\" ");
    assert_eq!(json[3]["value"], "~\"/tilde path\" ");
    assert!(json[0]["style"].is_null());
}

#[test]
fn xonsh_quoting_and_style() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![
        RawValue::new("plain"),
        RawValue::new("with space"),
        RawValue::new("back\\slash"),
        RawValue::styled("styled", "red"),
    ];
    let output = Shell::Xonsh.value("", &Meta::default(), &values);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["Value"], "back\\slash ");
    assert_eq!(json[1]["Value"], "plain ");
    assert_eq!(json[2]["Style"].as_str().unwrap().contains("fg:red"), true);
    assert_eq!(json[3]["Value"], "'with space' ");
}

#[test]
fn powershell_json_results() {
    let _guard = env_lock();
    clear_compline();
    unsafe { std::env::set_var("CARAPACE_COLOR", "disabled") };
    let values = vec![
        RawValue::described("plain", "the tip"),
        RawValue::new("sp ace"),
        RawValue::new(""),
    ];
    let output = Shell::PowerShell.value("", &Meta::default(), &values);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);
    assert_eq!(json[0]["CompletionText"], "plain ");
    assert_eq!(json[1]["CompletionText"], "'sp ace' ");
    assert_eq!(json[0]["ToolTip"], "the tip");
    // with tooltip enabled the description moves to the tooltip
    assert_eq!(json[0]["ListItemText"], "plain");
    unsafe { std::env::remove_var("CARAPACE_COLOR") };
}

#[test]
fn powershell_styled_tooltip_by_default() {
    let _guard = env_lock();
    clear_compline();
    unsafe { std::env::remove_var("CARAPACE_COLOR") };
    let values = vec![RawValue::described("plain", "the tip")];
    let output = Shell::PowerShell.value("", &Meta::default(), &values);
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(json[0]["ToolTip"].as_str().unwrap().contains("`e["));
    unsafe { std::env::remove_var("CARAPACE_COLOR") };
}
