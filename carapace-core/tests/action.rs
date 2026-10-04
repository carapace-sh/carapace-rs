use carapace_core::{Action, Context};

#[test]
fn values_invoke() {
    let invoked = Action::values(["a", "b", "c"]).invoke(&Context::default());
    let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
    assert_eq!(values, vec!["a", "b", "c"]);
}

#[test]
fn values_described() {
    let invoked = Action::values_described([("a", "first"), ("b", "second")])
        .invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "a");
    assert_eq!(invoked.values[0].display, "a");
    assert_eq!(invoked.values[0].description, "first");
}

#[test]
fn filter_and_retain() {
    let action = Action::values(["A", "B", "C"]);
    assert_eq!(
        values(action.clone().filter(["B"]).invoke(&Context::default())),
        vec!["A", "C"]
    );
    assert_eq!(
        values(action.retain(["A", "C"]).invoke(&Context::default())),
        vec!["A", "C"]
    );
}

#[test]
fn filter_args() {
    let action = Action::values(["A", "B", "C"]);
    let context = Context::default().args(["A"]);
    assert_eq!(values(action.filter_args().invoke(&context)), vec!["B", "C"]);
}

#[test]
fn prefix_and_suffix() {
    let action = Action::values(["melon", "drop"]);
    let invoked = action
        .clone()
        .prefix("water")
        .invoke(&Context::default().value(""));
    assert_eq!(
        values(invoked),
        vec!["watermelon", "waterdrop"]
    );

    // prefix mismatch suppresses all values
    let invoked = action.prefix("water").invoke(&Context::default().value("salt"));
    assert!(invoked.values.is_empty());
}

#[test]
fn prefix_filters_by_current_value() {
    let action = Action::values(["melon", "drop"]).prefix("water");
    // filtering by the current value happens later in the shell dispatch
    let invoked = action.invoke(&Context::default().value("waterme"));
    assert_eq!(values(invoked), vec!["watermelon", "waterdrop"]);
}

#[test]
fn suffix() {
    let invoked = Action::values(["apple"]).suffix("juice").invoke(&Context::default());
    assert_eq!(values(invoked), vec!["applejuice"]);
}

#[test]
fn style_and_tag() {
    let invoked = Action::values(["a", "b"])
        .style("blue")
        .tag("letters")
        .invoke(&Context::default());
    assert!(invoked.values.iter().all(|v| v.style == "blue"));
    assert!(invoked.values.iter().all(|v| v.tag == "letters"));
}

#[test]
fn callback_is_lazy() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let action = Action::callback(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Action::values(["x"])
    });
    assert!(action.is_callback());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    action.invoke(&Context::default());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn message() {
    let invoked = Action::message("nothing here").invoke(&Context::default());
    assert_eq!(invoked.meta.messages, vec!["nothing here"]);
    assert!(invoked.values.is_empty());
}

#[test]
fn unless() {
    let action = Action::values(["a", "b"]);
    assert!(action.clone().unless(true).invoke(&Context::default()).values.is_empty());
    assert_eq!(values(action.unless(false).invoke(&Context::default())), vec!["a", "b"]);
}

#[test]
fn unless_f() {
    let action = Action::values(["a", "b"]).unless_f(|c| !c.args.is_empty());
    assert!(action
        .invoke(&Context::default().args(["arg"]))
        .values
        .is_empty());
    assert_eq!(values(action.invoke(&Context::default())), vec!["a", "b"]);
}

#[test]
fn shift() {
    let action = Action::callback(|c| Action::values(c.args.clone()));
    let context = Context::default().args(["one", "two", "three"]);
    let invoked = action.shift(1).invoke(&context);
    assert_eq!(values(invoked), vec!["two", "three"]);
}

#[test]
fn nospace() {
    let invoked = Action::values(["a/"]).nospace(['/']).invoke(&Context::default());
    assert_eq!(invoked.meta.nospace, vec!['/']);
    let invoked = Action::values(["a"]).nospace([]).invoke(&Context::default());
    assert_eq!(invoked.meta.nospace, vec!['*']);
}

#[test]
fn multiparts_completes_last_segment() {
    let action = Action::multiparts("/", |c| {
        let values: &[&str] = match c.value.as_str() {
            "" => &["a", "b"],
            "a/" => &["x", "y"],
            _ => &["end"],
        };
        Action::values(values.iter().copied())
    });
    let invoked = action.invoke(&Context::default().value("a/"));
    assert_eq!(values(invoked), vec!["a/a", "a/b"]);

    let invoked = action.invoke(&Context::default().value("a/x"));
    assert_eq!(values(invoked), vec!["a/end"]);
}

#[test]
fn list() {
    let action = Action::values(["one", "two", "three"]).list(",");
    // filtering by the current value happens later in the shell dispatch
    let invoked = action.invoke(&Context::default().value("one,t"));
    assert_eq!(values(invoked), vec!["one,one", "one,two", "one,three"]);
}

#[test]
fn unique_list() {
    let action = Action::values(["one", "two", "three"]).unique_list(",");
    let invoked = action.invoke(&Context::default().value("one,t"));
    assert_eq!(values(invoked), vec!["one,two", "one,three"]);
    let invoked = action.invoke(&Context::default().value("one,three,t"));
    assert_eq!(values(invoked), vec!["one,three,two"]);
}

#[test]
fn chained_meta_merges_through_callbacks() {
    let invoked = Action::callback(|_| Action::message("inner").nospace([':']))
        .usage("outer usage")
        .invoke(&Context::default());
    assert_eq!(invoked.meta.messages, vec!["inner"]);
    assert_eq!(invoked.meta.usage.as_deref(), Some("outer usage"));
    assert_eq!(invoked.meta.nospace, vec![':']);
}

#[test]
fn invoked_merge_dedups() {
    let a = Action::values(["a", "b"]).invoke(&Context::default());
    let b = Action::values(["b", "c"]).invoke(&Context::default());
    let merged = carapace_core::action::merge_all([a, b]);
    assert_eq!(values(merged), vec!["a", "b", "c"]);
}

fn values(invoked: carapace_core::InvokedAction) -> Vec<String> {
    invoked.values.into_iter().map(|v| v.value).collect()
}
