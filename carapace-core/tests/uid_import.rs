use carapace_core::{Action, Context, uid};

#[test]
fn uid_command_and_flag() {
    assert_eq!(uid::command("git", &["branch"]), "cmd://git/branch");
    assert_eq!(
        uid::flag("git", &["branch"], "force"),
        "cmd://git/branch?flag=force"
    );
    assert_eq!(uid::command("app", &["sub", "deep"]), "cmd://app/sub/deep");
}

#[test]
fn uid_path_escape() {
    assert_eq!(uid::path_escape("users/admin"), "users-admin");
    assert_eq!(uid::path_escape("a b"), "a%20b");
}

#[test]
fn uid_with_opts_skips_empty_values() {
    assert_eq!(
        uid::with_opts("file", "host", "value", &[("flag", "x"), ("empty", "")]),
        "file://host/value?flag=x"
    );
}

#[test]
fn uid_modifier_sets_value_uids() {
    let action = Action::values(["a", "b/c"]).uid("cmd", "app", vec![("flag", "force".into())]);
    let invoked = action.invoke(&Context::default());
    assert_eq!(
        invoked.values[0].uid.as_deref(),
        Some("cmd://app/a?flag=force")
    );
    assert_eq!(
        invoked.values[1].uid.as_deref(),
        Some("cmd://app/b-c?flag=force")
    );
}

#[test]
fn import_roundtrips_export() {
    let source = Action::values_described([("a", "first")]).invoke(&Context::default());
    let json = carapace_core::export::Export::from_parts(&source.meta, &source.values).to_json();
    let action = Action::import(&json);
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "a");
    assert_eq!(invoked.values[0].description, "first");
}

#[test]
fn import_invalid_json_becomes_message() {
    let invoked = Action::import("not json").invoke(&Context::default());
    assert!(!invoked.meta.messages.is_empty());
    assert!(invoked.values.is_empty());
}

#[test]
fn export_sorts_by_value() {
    let invoked = Action::values(["b", "a"]).invoke(&Context::default());
    let json = carapace_core::export::Export::from_parts(&invoked.meta, &invoked.values).to_json();
    assert!(json.find("\"a\"").unwrap() < json.find("\"b\"").unwrap());
}
