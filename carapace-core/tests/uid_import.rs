use carapace_core::{Action, Context, uid};

#[test]
fn uid_command_and_flag() {
    assert_eq!(uid::command("git", &["branch"]), "cmd://git/branch");
    assert_eq!(
        uid::flag("git", &["branch"], "force"),
        "cmd://git/branch?flag=force"
    );
    assert_eq!(uid::command("app", &["sub", "deep"]), "cmd://app/sub/deep");
    // an empty path renders without the trailing separator (Go url.URL)
    assert_eq!(uid::command("app", &[] as &[&str]), "cmd://app");
    assert_eq!(
        uid::flag("app", &[] as &[&str], "force"),
        "cmd://app?flag=force"
    );
}

#[test]
fn uid_path_escape() {
    // slashes are preserved as separators (Go uid.PathEscape)
    assert_eq!(uid::path_escape("users/admin"), "users/admin");
    assert_eq!(uid::path_escape("/tmp"), "/tmp");
    assert_eq!(uid::path_escape("a b"), "a%20b");
    // segment escaping keeps the reserved `: @ & = + $`, escapes `; , ?`
    assert_eq!(uid::path_escape("a;b,c?d"), "a%3Bb%2Cc%3Fd");
    assert_eq!(uid::path_escape("a@b&c=d+e$f"), "a@b&c=d+e$f");
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
        Some("cmd://app/b/c?flag=force")
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

#[test]
fn import_parses_go_carapace_export() {
    // shape emitted by Go carapace: flattened meta, matcher strings,
    // lowercase value keys
    let json = r#"{
        "version": "v0.7.0",
        "messages": ["oops"],
        "noprefix": "ab",
        "nospace": "/=",
        "usage": "the usage",
        "queries": ["cmd://app/sub?flag=force&C_VALUE=x"],
        "values": [
            {"value": "a", "display": "a", "description": "first"},
            {"value": "b/c", "display": "b/c", "style": "blue", "uid": "file://x/y"}
        ]
    }"#;
    let invoked = Action::import(json).invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "a");
    assert_eq!(invoked.values[0].description, "first");
    assert_eq!(invoked.values[1].style, "blue");
    assert_eq!(invoked.meta.messages, vec!["oops"]);
    assert_eq!(invoked.meta.nospace, vec!['/', '=']);
    assert_eq!(invoked.meta.no_prefix, vec!['a', 'b']);
    assert_eq!(invoked.meta.usage.as_deref(), Some("the usage"));
    assert_eq!(invoked.meta.queries[0].scheme, "cmd");
    assert_eq!(invoked.meta.queries[0].host, "app");
    assert_eq!(invoked.meta.queries[0].path, "sub");
}

#[test]
fn export_json_matches_go_shape() {
    let meta = carapace_core::Meta::default()
        .nospace(['/'])
        .no_prefix(['a']);
    let invoked = Action::values_described([("a", "first")]).invoke(&Context::default());
    let json = carapace_core::export::Export::from_parts(&meta, &invoked.values).to_json();
    assert!(!json.contains("\"Meta\""), "meta must be flattened: {json}");
    assert!(json.contains("\"values\":"), "{json}");
    assert!(json.contains("\"value\":\"a\""), "{json}");
    assert!(json.contains("\"nospace\":\"/\""), "{json}");
    assert!(json.contains("\"noprefix\":\"a\""), "{json}");
}
