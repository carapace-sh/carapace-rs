use carapace_core::export::Export;
use carapace_core::{Action, Context, Meta, RawValue};

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
    let output = carapace_core::shell::Shell::Export.value(
        "",
        &invoked.meta,
        &invoked.values,
        &carapace_core::shell::bash::Context::default(),
    );
    assert!(Export::from_json(&output).is_some());
}

#[test]
fn bash_plain_values() {
    use carapace_core::shell::{Shell, bash};
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
    let output = carapace_core::shell::Shell::Bash.value(
        "",
        &Meta::default(),
        &vec![value],
        &carapace_core::shell::bash::Context::default(),
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
    let output = carapace_core::shell::Shell::Bash.value(
        "o",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
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
    let output = carapace_core::shell::Shell::Bash.value(
        "on",
        &Meta::default(),
        &values,
        &carapace_core::shell::bash::Context::default(),
    );
    let (nospace, data) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true");
    assert_eq!(data, "one\nonly");
}
