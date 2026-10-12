use carapace_rs::export::Export;

/// Serializes tests that touch process environment variables.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
use carapace_rs::{Action, Context, Meta, RawValue};

#[test]
fn export_roundtrip() {
    let meta = Meta::default().message("oops").nospace(['/']);
    let values = vec![
        RawValue::described("a", "first"),
        RawValue::styled_described("b", "second", "blue").nospace(true),
    ];
    let export = Export::from_parts(&meta, &values);
    let json = export.to_json();
    let parsed = Export::from_json(&json).unwrap();
    assert_eq!(parsed, export);
    let restored = parsed.to_raw_values();
    assert_eq!(restored[0].value, "a");
    assert_eq!(restored[0].description, "first");
    assert_eq!(restored[1].style, "blue");
    assert_eq!(restored[1].nospace, Some(true));
}

#[test]
fn shell_export_value_is_json() {
    let action = Action::values(["x"]);
    let invoked = action.invoke(&Context::default());
    let output = carapace_rs::shell::Shell::Export.value(
        "",
        &invoked.meta,
        &invoked.values,
        &carapace_rs::shell::bash::Context::default(),
    );
    assert!(Export::from_json(&output).is_some());
}

#[test]
fn bash_plain_values() {
    use carapace_rs::shell::{Shell, bash};
    let output = Shell::Bash.value(
        "",
        &Meta::default(),
        &vec![
            RawValue::new("alpha"),
            RawValue::described("spaced value", "desc"),
        ],
        &bash::Context::default(),
    );
    let (nospace, data) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "false");
    let mut lines = data.lines();
    assert_eq!(lines.next().unwrap(), "alpha");
    assert_eq!(lines.next().unwrap(), "\"spaced value\"");
    assert!(lines.next().is_none());
    let _ = bash::Context::default();
}

#[test]
fn bash_nospace_marker() {
    let value = RawValue::new("dir/").nospace(true);
    let output = carapace_rs::shell::Shell::Bash.value(
        "",
        &Meta::default(),
        &vec![value],
        &carapace_rs::shell::bash::Context::default(),
    );
    let (nospace, _) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
}

#[test]
fn bash_common_prefix_collapse() {
    let values = vec![
        RawValue::described("one", "first"),
        RawValue::described("only", "second"),
    ];
    let output = carapace_rs::shell::Shell::Bash.value(
        "o",
        &Meta::default(),
        &values,
        &carapace_rs::shell::bash::Context::default(),
    );
    let (nospace, data) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
    assert_eq!(data, "on");
}

#[test]
fn bash_common_prefix_space_hack() {
    let values = vec![
        RawValue::described("one", "first"),
        RawValue::described("only", "second"),
    ];
    // current word already equals the common prefix: space-prefix the display,
    // values themselves are still rendered in normal (non-listing) mode
    let output = carapace_rs::shell::Shell::Bash.value(
        "on",
        &Meta::default(),
        &values,
        &carapace_rs::shell::bash::Context::default(),
    );
    let (nospace, data) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
    assert_eq!(data, "one\nonly");
}

#[test]
fn nushell_patch_dequotes_args() {
    let patched = carapace_rs::shell::nushell::patch(vec![
        "\"a b\"".into(),
        "'single'".into(),
        "`raw string`".into(),
        "plain".into(),
        "\"".into(), // lone quote: not dequotable, kept
    ]);
    assert_eq!(patched, vec!["a b", "single", "raw string", "plain", "\""]);
}

#[test]
fn cmd_clink_patch_relexes_compline() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "myapp --flag \"a b\" tail") };
    let patched = carapace_rs::shell::simple::cmd_clink::patch(vec!["ignored".into()]);
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
    assert_eq!(patched, vec!["--flag", "a b", "tail"]);
}

#[test]
fn cmd_clink_patch_honors_caret_escape() {
    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_COMPLINE", "myapp one^ two") };
    let patched = carapace_rs::shell::simple::cmd_clink::patch(vec![]);
    unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
    assert_eq!(patched, vec!["one two"]);
}
