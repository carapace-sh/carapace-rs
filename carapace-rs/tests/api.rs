//! Black-box contract tests: this file pins the public API surface of the
//! engine. It compiles against public items only, so it fails the build if
//! the encapsulation boundary leaks — `Action` and `InvokedAction` internals
//! are reachable exclusively through the modifier functions and the export
//! wire format (`InvokedAction::to_export`, Go's `MarshalJSON` analog).
//!
//! Behavior coverage lives in the `#[cfg(test)]` modules beside the code
//! (white-box, Go's in-package test style) and in the sandbox tests that
//! exercise the engine through its adapters.

use carapace_rs::{Action, Command, Context, Flag, Positional, traverse};

#[test]
fn action_results_are_observable_through_the_export_format() {
    let invoked = Action::values(["b", "a"])
        .nospace(['/'])
        .invoke(&Context::default());
    let export = invoked.to_export();
    // values arrive sorted by the wire format (Go MarshalJSON)
    assert_eq!(
        export
            .values
            .iter()
            .map(|v| v.value.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(export.meta.nospace, "/");
}

#[test]
fn export_json_roundtrips_through_import() {
    let invoked =
        Action::values_described([("a", "first"), ("b", "second")]).invoke(&Context::default());
    let json = invoked.to_export().to_json();
    let restored = Action::import(&json).invoke(&Context::default());
    assert_eq!(restored.to_export().to_json(), json);
}

#[test]
fn traverses_the_public_model() {
    let model = Command::new("app")
        .flag(Flag {
            id: "verbose".into(),
            name: "verbose".into(),
            shorthand: Some('v'),
            ..Default::default()
        })
        .positional(Positional {
            name: "target".into(),
            completion: Some(Action::values(["t1", "t2"])),
            ..Default::default()
        });
    let completion = traverse(&model, vec![String::new()]);
    let invoked = completion.action.invoke(&completion.context);
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["t1", "t2"]);
}

#[test]
fn parsed_flag_keys_carry_flag_ids_not_names() {
    use std::sync::{Arc, Mutex};

    // an id and a name that differ, like clap's arg id vs `--work-tree`
    let recorded = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorder = recorded.clone();
    let model = Command::new("app")
        .flag(Flag {
            id: "work_tree".into(),
            name: "work-tree".into(),
            takes_value: true,
            ..Default::default()
        })
        .positional(Positional {
            name: "target".into(),
            completion: Some(Action::callback(move |c| {
                recorder
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .extend(c.parsed_flags.iter().map(|(key, _)| key.clone()));
                Action::default()
            })),
            ..Default::default()
        });

    let completion = traverse(
        &model,
        vec!["--work-tree".into(), "x".into(), String::new()],
    );
    let _ = completion.action.invoke(&completion.context);

    // the callback context keys on the flag id, never on the long name
    let keys = recorded
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(*keys, vec!["work_tree".to_owned()]);
}
