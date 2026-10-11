use carapace_core::shell::{Shell, zsh};
use carapace_core::{Action, Context, Meta, RawValue};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serializes tests that touch CARAPACE_* environment variables.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    let output = Shell::Fish.value(
        "",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "apple\ta fruit");
    assert_eq!(lines[2], "cherry\tno desc");
}

#[test]
fn fish_description_length_env() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_DESCRIPTION_LENGTH", "5") };
    let output = Shell::Fish.value(
        "",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    assert_eq!(output.lines().next().unwrap(), "apple\ta ...");
}

#[test]
fn prefix_filtering() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let output = Shell::Fish.value(
        "b",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    assert_eq!(output, "banana\ta fruit");
}

#[test]
fn messages_integrated_as_values() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let meta = Meta::default().message("something broke");
    let output = Shell::Fish.value(
        "",
        &meta,
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.iter().any(|line| line.contains("something broke")));
    assert!(lines.iter().any(|line| line.starts_with("ERR\t")));
}

#[test]
fn message_nospace_propagation() {
    let meta = Meta::default().message("oops");
    let output = Shell::Bash.value(
        "",
        &meta,
        &vec![RawValue::new("x")],
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::Zsh.value(
        "",
        &Meta::default(),
        &tagged,
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::Zsh.value(
        "",
        &Meta::default(),
        &tagged,
        &carapace_core::shell::bash::Context::default(),
    );
    let groups = zsh_groups(&output);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "flags");
    assert_eq!(groups[1].0, "default");
}

#[test]
fn zsh_double_quote_state() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "cmd \"ap") };
    let output = Shell::Zsh.value(
        "ap",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
    let groups = zsh_groups(&output);
    assert_eq!(groups[0].2[0], "apple\" ");
}

#[test]
fn zsh_single_quote_state() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "cmd 'ap") };
    let output = Shell::Zsh.value(
        "ap",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
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
        &carapace_core::shell::bash::Context::default(),
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
    let output = Shell::Zsh.value(
        "",
        &meta,
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert!(sections[1].contains("broken"));
}

#[test]
fn zsh_zstyle_section() {
    let _guard = env_lock();
    clear_compline();
    let output = Shell::Zsh.value(
        "",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert!(sections[0].contains("=(#b)(apple)"));
}

#[test]
fn sorted_by_display() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CARAPACE_DESCRIPTION_LENGTH") };
    let unsorted = vec![RawValue::new("b"), RawValue::new("a")];
    let output = Shell::Fish.value(
        "",
        &Meta::default(),
        &unsorted,
        &carapace_core::shell::bash::Context::default(),
    );
    assert_eq!(output, "a\t\nb\t");
}

#[test]
fn export_shell_native_messages() {
    let meta = Meta::default().message("kept in meta");
    let output = Shell::Export.value(
        "",
        &meta,
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    let parsed = carapace_core::export::Export::from_json(&output).unwrap();
    assert_eq!(parsed.meta.messages, vec!["kept in meta"]);
    assert!(parsed.values.iter().all(|v| !v.value.starts_with("ERR")));
}

#[test]
fn nospace_env_propagation() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_NOSPACE", "/") };
    let output = Shell::Bash.value(
        "",
        &Meta::default(),
        &vec![RawValue::new("dir/")],
        &carapace_core::shell::bash::Context::default(),
    );
    unsafe { std::env::remove_var("CARAPACE_NOSPACE") };
    let (nospace, _) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
}

#[test]
fn multiparts_nospace_through_dispatch() {
    let action = Action::multiparts("/", |c| {
        let values: &[&str] = if c.value.is_empty() {
            &["home", "etc"]
        } else {
            &["x"]
        };
        Action::values(values.iter().copied())
    });
    let invoked = action.invoke(&Context::default().value(""));
    let output = Shell::Bash.value(
        "",
        &invoked.meta,
        &invoked.values,
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::Elvish.value(
        "",
        &Meta::default(),
        &vec![value, nospace_value],
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::Elvish.value(
        "",
        &meta,
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::Nushell.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::Xonsh.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["Value"], "back\\slash ");
    assert_eq!(json[1]["Value"], "plain ");
    assert!(json[2]["Style"].as_str().unwrap().contains("fg:red"));
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
    let output = Shell::PowerShell.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
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
    let output = Shell::PowerShell.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(json[0]["ToolTip"].as_str().unwrap().contains("`e["));
    unsafe { std::env::remove_var("CARAPACE_COLOR") };
}

#[test]
fn oil_inline_nospace_indicator() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![RawValue::new("dir/").nospace(true), RawValue::new("file")];
    let output = Shell::Oil.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    assert_eq!(output, "dir/\u{1}\nfile");
}

#[test]
fn oil_single_value_plain() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![RawValue::described("only", "desc")];
    let output = Shell::Oil.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    assert_eq!(output, "only");
}

#[test]
fn clink_tab_separated_with_appendchar() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![
        RawValue::described("a", "desc"),
        RawValue::new("dir/").nospace(true),
    ];
    let output = Shell::CmdClink.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "a\ta\tdesc\t ");
    assert_eq!(lines[1], "dir/\tdir/\t\t");
}

#[test]
fn ion_json_suggestions() {
    let _guard = env_lock();
    clear_compline();
    let output = Shell::Ion.value(
        "",
        &Meta::default(),
        &values(),
        &carapace_core::shell::bash::Context::default(),
    );
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["Value"], "apple ");
    assert_eq!(json[2]["Value"], "cherry ");
    assert_eq!(json[0]["Display"], "apple");
}

#[test]
fn bash_ble_suffix_field() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![RawValue::new("x"), RawValue::new("dir/").nospace(true)];
    let output = Shell::BashBle.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[1], "x\tx\u{1c}\u{1c} \u{1c}");
    assert_eq!(lines[0], "dir/\tdir/\u{1c}\u{1c}\u{1c}");
}

#[test]
fn tcsh_escapes_and_descriptions() {
    let _guard = env_lock();
    clear_compline();
    let values = vec![RawValue::new("a b"), RawValue::described("x", "two words")];
    let output = Shell::Tcsh.value(
        "",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "a\\ b");
    assert!(lines[1].starts_with("x_("));
    assert!(lines[1].contains("two"));
}

#[test]
fn tcsh_common_prefix_collapse() {
    let _guard = env_lock();
    clear_compline();
    unsafe { std::env::remove_var("COMP_WORDBREAKS") };
    let values = vec![RawValue::new("one"), RawValue::new("only")];
    let output = Shell::Tcsh.value(
        "o",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    assert_eq!(output, "on");
}

#[test]
fn files_suffix_filters_candidate_names() {
    let dir = std::env::temp_dir().join("carapace-files-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("readme.md"), "").unwrap();
    std::fs::write(dir.join("script.sh"), "").unwrap();
    let action = Action::files([".sh"]);
    let invoked = action.invoke(&Context::default().dir(&dir).value(""));
    let names: Vec<String> = invoked
        .values
        .iter()
        .map(|value| value.value.rsplit('/').next().unwrap().to_owned())
        .collect();
    assert_eq!(names, vec!["script.sh"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn zsh_noprefix_flag_in_output() {
    let _guard = env_lock();
    clear_compline();

    let meta = carapace_core::Meta::default().no_prefix(['p']);
    let output = zsh::action_raw_values("a", &meta, &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert_eq!(
        sections[2], "true",
        "newly inserted part of apple matches no-prefix"
    );

    let meta = carapace_core::Meta::default().no_prefix(['z']);
    let output = zsh::action_raw_values("a", &meta, &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert_eq!(sections[2], "false", "no value starts with z");

    let meta = carapace_core::Meta::default();
    let output = zsh::action_raw_values("a", &meta, &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert_eq!(sections[2], "false", "unset no-prefix never matches");
}

#[test]
fn zsh_raw_current_word_keeps_quoted_spaces() {
    // The raw current word comes from the shlex re-lex of CARAPACE_COMPLINE:
    // a quoted word containing a space stays whole ('a b' → Quoting state
    // with the full raw token), where a naive whitespace split would see 'a.
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "cmd 'a b") };

    let output = zsh::action_raw_values("a b", &carapace_core::Meta::default(), &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    // values end with the closing quote (Quoting state)
    assert!(
        sections[3].contains("apple'"),
        "quoted word must close the quote: {sections:?}"
    );

    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };

    // without compline the state is default (no quote suffix)
    let output = zsh::action_raw_values("a", &carapace_core::Meta::default(), &values());
    let sections: Vec<&str> = output.split('\u{1}').collect();
    assert!(
        !sections[3].contains("apple'"),
        "default state must not add a quote: {sections:?}"
    );
}

#[test]
fn cmd_clink_patch_relexes_compline() {
    // Port of Go cmd_clink TestPatch: CARAPACE_COMPLINE is re-lexed with
    // cmd.exe rules (^ escaping, " quoting, literal ') and the binary name
    // is dropped; the remaining words feed the traversal (Go's args[2:]).
    let _guard = env_lock();
    unsafe {
        std::env::set_var(
            "CARAPACE_COMPLINE",
            r#"example C:\path\to "a b" a^b 'single'"#,
        )
    };
    let words = carapace_core::shell::simple::cmd_clink::patch(vec!["cmd-clink".to_owned()]);
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
    assert_eq!(
        words,
        vec![
            r"C:\path\to".to_owned(),
            "a b".to_owned(),
            "ab".to_owned(),
            "'single'".to_owned()
        ]
    );

    // without the env var the args pass through unchanged
    let words = carapace_core::shell::simple::cmd_clink::patch(vec!["cmd-clink".to_owned()]);
    assert_eq!(words, vec!["cmd-clink".to_owned()]);
}

#[test]
fn files_carry_ls_colors_styles() {
    // Go ActionFiles' StyleF(style.ForPath): colors from LS_COLORS
    let _guard = env_lock();
    clear_compline();

    let base = std::env::temp_dir().join(format!("carapace-lsstyle-{}", std::process::id()));
    std::fs::create_dir_all(base.join("subdir")).unwrap();
    std::fs::write(base.join("readme.md"), "").unwrap();

    let action = Action::files(Vec::<String>::new());
    let context = Context::default()
        .dir(base.clone())
        .env("LS_COLORS", "di=01;35:*.md=01;32");
    let mut values = action.invoke(&context).values;
    values.sort_by(|a, b| a.value.cmp(&b.value));

    let styles: std::collections::HashMap<String, String> =
        values.into_iter().map(|v| (v.value, v.style)).collect();
    assert_eq!(styles["readme.md"], "green bold");
    assert_eq!(styles["subdir/"], "magenta bold");

    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn for_path_ext_ignores_file_features() {
    // Go style.ForPathExt: extension only, directory fallback
    let context = Context::default().env("LS_COLORS", "di=01;34:*.md=01;31");
    assert_eq!(
        carapace_core::shell::for_path_ext(&context, "file.md"),
        "red bold"
    );
    assert_eq!(
        carapace_core::shell::for_path_ext(&context, "anything"),
        "blue bold"
    );
    assert_eq!(
        carapace_core::shell::for_extension(&context, "md"),
        "red bold"
    );
}
